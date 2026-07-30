module push_policy
  use, intrinsic :: iso_c_binding, only: c_double, c_int
  implicit none
contains
  function arach_push_supervision_score(features, count) result(score) bind(C)
    real(c_double), intent(in) :: features(*)
    integer(c_int), value, intent(in) :: count
    real(c_double) :: score
    real(c_double) :: criticality, health, readiness, pressure

    if (count < 4_c_int) then
      score = 0.0_c_double
      return
    end if
    criticality = max(0.0_c_double, min(1.0_c_double, features(1)))
    health = max(0.0_c_double, min(1.0_c_double, features(2)))
    readiness = max(0.0_c_double, min(1.0_c_double, features(3)))
    pressure = max(0.0_c_double, min(1.0_c_double, features(4)))
    score = criticality * 0.40_c_double + health * 0.20_c_double &
      + readiness * 0.40_c_double - pressure * 0.25_c_double
    score = max(0.0_c_double, min(1.0_c_double, score))
  end function arach_push_supervision_score
end module push_policy
